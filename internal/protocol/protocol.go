package protocol

import (
	"context"
	"errors"

	"github.com/ManethonSega/MechKeyMesh/internal/domain"
)

var (
	ErrNotFound            = errors.New("supported keyboard not found")
	ErrUnsupportedPlatform = errors.New("keyboard transport is not supported on this platform")
	ErrNotReady            = errors.New("keyboard is not ready")
)

// Device is the minimum interface implemented by every keyboard driver.
type Device interface {
	Info() domain.DeviceInfo
	Close() error
}

// Factory discovers and opens one device family.
type Factory interface {
	Open(ctx context.Context) (Device, error)
}

// LightingConfig is a vendor-neutral lighting request for onboard effects.
type LightingConfig struct {
	Mode       int  `json:"mode"`
	R          byte `json:"r"`
	G          byte `json:"g"`
	B          byte `json:"b"`
	Brightness byte `json:"brightness"`
	Speed      byte `json:"speed"`
	Direction  byte `json:"direction"`
	Colorful   bool `json:"colorful"`
}

// LightingDevice is implemented by keyboards with persistent onboard lighting effects.
type LightingDevice interface {
	Device
	SetLighting(ctx context.Context, cfg LightingConfig) error
}
