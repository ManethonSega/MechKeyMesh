package device

import (
	"context"
	"errors"
	"fmt"
	"sync"
	"time"

	"github.com/ManethonSega/MechKeyMesh/internal/domain"
	"github.com/ManethonSega/MechKeyMesh/internal/logging"
	"github.com/ManethonSega/MechKeyMesh/internal/protocol"
)

type operation struct {
	name    string
	timeout time.Duration
	run     func(context.Context, protocol.Device) error
	result  chan error
}

// Manager owns the active device and is the only place allowed to execute
// hardware operations. All calls are serialized through queue.
type Manager struct {
	factory protocol.Factory
	log     *logging.Ring

	mu     sync.RWMutex
	status domain.DeviceStatus
	dev    protocol.Device

	queue     chan operation
	reconnect chan struct{}
}

func NewManager(factory protocol.Factory, log *logging.Ring) *Manager {
	return &Manager{
		factory: factory,
		log:     log,
		status: domain.DeviceStatus{
			State:       domain.StateDisconnected,
			LastChanged: time.Now(),
		},
		queue:     make(chan operation, 32),
		reconnect: make(chan struct{}, 1),
	}
}

func (m *Manager) Status() domain.DeviceStatus {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.status
}

func (m *Manager) setStatus(state domain.DeviceState, info domain.DeviceInfo, err error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.status.State = state
	m.status.Info = info
	m.status.LastChanged = time.Now()
	if err != nil {
		m.status.LastError = err.Error()
	} else {
		m.status.LastError = ""
	}
}

func (m *Manager) Start(ctx context.Context) {
	m.log.Add("info", "Device manager started")
	go m.operationLoop(ctx)
	go m.discoveryLoop(ctx)
}

func (m *Manager) Close() {
	m.mu.Lock()
	dev := m.dev
	m.dev = nil
	m.mu.Unlock()
	if dev != nil {
		_ = dev.Close()
	}
}

func (m *Manager) Reconnect() {
	select {
	case m.reconnect <- struct{}{}:
	default:
	}
}

func (m *Manager) discoveryLoop(ctx context.Context) {
	ticker := time.NewTicker(2 * time.Second)
	defer ticker.Stop()

	m.tryOpen(ctx)
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			if !m.hasDevice() {
				m.tryOpen(ctx)
			}
		case <-m.reconnect:
			m.dropDevice(nil)
			m.tryOpen(ctx)
		}
	}
}

func (m *Manager) hasDevice() bool {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.dev != nil
}

func (m *Manager) tryOpen(ctx context.Context) {
	if m.hasDevice() {
		return
	}
	m.setStatus(domain.StateDetecting, domain.DeviceInfo{}, nil)

	openCtx, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	m.setStatus(domain.StateOpening, domain.DeviceInfo{}, nil)
	dev, err := m.factory.Open(openCtx)
	if err != nil {
		switch {
		case errors.Is(err, protocol.ErrUnsupportedPlatform):
			m.setStatus(domain.StateUnsupported, domain.DeviceInfo{}, err)
		case errors.Is(err, protocol.ErrNotFound):
			m.setStatus(domain.StateDisconnected, domain.DeviceInfo{}, nil)
		default:
			m.setStatus(domain.StateDisconnected, domain.DeviceInfo{}, err)
			m.log.Add("warning", "Device open failed: %v", err)
		}
		return
	}

	m.mu.Lock()
	m.dev = dev
	m.mu.Unlock()
	m.setStatus(domain.StateReady, dev.Info(), nil)
	m.log.Add("info", "%s connected via %s", dev.Info().Name, dev.Info().Connection)
}

func (m *Manager) dropDevice(cause error) {
	m.mu.Lock()
	dev := m.dev
	m.dev = nil
	info := m.status.Info
	m.mu.Unlock()

	if dev != nil {
		_ = dev.Close()
	}
	if cause != nil {
		m.setStatus(domain.StateRecovery, info, cause)
		m.log.Add("warning", "Device entered recovery: %v", cause)
	} else {
		m.setStatus(domain.StateDisconnected, domain.DeviceInfo{}, nil)
	}
}

func (m *Manager) Submit(ctx context.Context, name string, timeout time.Duration, run func(context.Context, protocol.Device) error) error {
	if timeout <= 0 {
		timeout = 3 * time.Second
	}
	op := operation{name: name, timeout: timeout, run: run, result: make(chan error, 1)}
	select {
	case m.queue <- op:
	case <-ctx.Done():
		return ctx.Err()
	}

	select {
	case err := <-op.result:
		return err
	case <-ctx.Done():
		return ctx.Err()
	}
}

func (m *Manager) operationLoop(ctx context.Context) {
	for {
		select {
		case <-ctx.Done():
			return
		case op := <-m.queue:
			op.result <- m.execute(ctx, op)
		}
	}
}

func (m *Manager) execute(parent context.Context, op operation) error {
	m.mu.RLock()
	dev := m.dev
	info := m.status.Info
	m.mu.RUnlock()
	if dev == nil {
		return protocol.ErrNotReady
	}

	m.setStatus(domain.StateBusy, info, nil)
	m.log.Add("info", "Operation started: %s", op.name)

	ctx, cancel := context.WithTimeout(parent, op.timeout)
	defer cancel()
	if err := op.run(ctx, dev); err != nil {
		wrapped := fmt.Errorf("%s: %w", op.name, err)
		m.dropDevice(wrapped)
		return wrapped
	}

	m.setStatus(domain.StateReady, dev.Info(), nil)
	m.log.Add("info", "Operation completed: %s", op.name)
	return nil
}
