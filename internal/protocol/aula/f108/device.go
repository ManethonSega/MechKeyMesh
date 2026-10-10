package f108

import (
	"context"
	"fmt"
	"time"

	"github.com/ManethonSega/MechKeyMesh/internal/domain"
	"github.com/ManethonSega/MechKeyMesh/internal/protocol"
)

const (
	VendorID     uint16 = 0x0C45
	ProductID    uint16 = 0x800A
	reportSize          = 64
	commandDelay        = 35 * time.Millisecond
)

// Factory opens wired AULA F108 Pro devices.
type Factory struct{}

func (Factory) Open(ctx context.Context) (protocol.Device, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	t, err := openTransport()
	if err != nil {
		return nil, err
	}
	return &Device{transport: t}, nil
}

// Device is a wired AULA F108 Pro.
type Device struct{ transport transport }

func (d *Device) Info() domain.DeviceInfo {
	return domain.DeviceInfo{
		DriverID:   "aula.f108pro.usb",
		Name:       "AULA F108 Pro",
		Vendor:     "AULA",
		Connection: "USB",
		VendorID:   VendorID,
		ProductID:  ProductID,
		Capabilities: []string{
			"keys", "fn-layer", "macros", "hardware-rgb", "per-key-rgb",
			"live-rgb", "music", "lcd", "clock", "device-settings",
		},
	}
}

func (d *Device) Close() error {
	if d.transport == nil {
		return nil
	}
	return d.transport.Close()
}

func (d *Device) sendCommand(ctx context.Context, payload [reportSize]byte, readback bool) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if err := d.transport.SetFeatureReport(payload); err != nil {
		return err
	}
	if err := waitContext(ctx, commandDelay); err != nil {
		return err
	}
	if readback {
		ack, err := d.transport.GetFeatureReport()
		if err != nil {
			return fmt.Errorf("readback: %w", err)
		}
		if ack[3] != 0x01 {
			return fmt.Errorf("unexpected device acknowledgment byte 3: 0x%02X", ack[3])
		}
		if err := waitContext(ctx, commandDelay); err != nil {
			return err
		}
	}
	return nil
}

func (d *Device) beginTransaction(ctx context.Context) error {
	var p [reportSize]byte
	p[0], p[1] = 0x04, 0x18
	return d.sendCommand(ctx, p, true)
}

func (d *Device) lightingInit(ctx context.Context) error {
	var p [reportSize]byte
	p[0], p[1], p[8] = 0x04, 0x13, 0x01
	return d.sendCommand(ctx, p, true)
}

func (d *Device) applyTransaction(ctx context.Context) error {
	var p [reportSize]byte
	p[0], p[1] = 0x04, 0x02
	return d.sendCommand(ctx, p, true)
}

func (d *Device) finalizeTransaction(ctx context.Context) error {
	var p [reportSize]byte
	p[0], p[1] = 0x04, 0xF0
	return d.sendCommand(ctx, p, false)
}

func waitContext(ctx context.Context, d time.Duration) error {
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-t.C:
		return nil
	}
}
