package device

import (
	"context"
	"sync"
	"testing"
	"time"

	"github.com/ManethonSega/MechKeyMesh/internal/domain"
	"github.com/ManethonSega/MechKeyMesh/internal/logging"
	"github.com/ManethonSega/MechKeyMesh/internal/protocol"
)

type fakeDevice struct{}

func (fakeDevice) Info() domain.DeviceInfo {
	return domain.DeviceInfo{Name: "Fake", Connection: "USB"}
}
func (fakeDevice) Close() error { return nil }

type fakeFactory struct{ dev protocol.Device }

func (f fakeFactory) Open(context.Context) (protocol.Device, error) { return f.dev, nil }

func TestManagerSerializesOperations(t *testing.T) {
	m := NewManager(fakeFactory{dev: fakeDevice{}}, logging.NewRing(16))
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	m.Start(ctx)
	defer m.Close()

	deadline := time.Now().Add(time.Second)
	for m.Status().State != domain.StateReady && time.Now().Before(deadline) {
		time.Sleep(time.Millisecond)
	}
	if m.Status().State != domain.StateReady {
		t.Fatalf("manager did not become ready: %#v", m.Status())
	}

	var mu sync.Mutex
	active := 0
	maxActive := 0
	run := func(ctx context.Context, _ protocol.Device) error {
		mu.Lock()
		active++
		if active > maxActive {
			maxActive = active
		}
		mu.Unlock()
		time.Sleep(15 * time.Millisecond)
		mu.Lock()
		active--
		mu.Unlock()
		return nil
	}

	var wg sync.WaitGroup
	for i := 0; i < 4; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if err := m.Submit(ctx, "test", time.Second, run); err != nil {
				t.Errorf("submit: %v", err)
			}
		}()
	}
	wg.Wait()
	if maxActive != 1 {
		t.Fatalf("max concurrent operations = %d, want 1", maxActive)
	}
}
