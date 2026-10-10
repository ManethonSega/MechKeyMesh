package f108

import (
	"context"
	"fmt"

	"github.com/ManethonSega/MechKeyMesh/internal/protocol"
)

// LightingMode is an onboard AULA F108 Pro lighting effect ID.
type LightingMode int

const (
	ModeOff LightingMode = iota
	ModeStatic
	ModeSingleOn
	ModeSingleOff
	ModeGlittering
	ModeFalling
	ModeColourful
	ModeBreath
	ModeSpectrum
	ModeOutward
	ModeScrolling
	ModeRolling
	ModeRotating
	ModeExplode
	ModeLaunch
	ModeRipples
	ModeFlowing
	ModePulsating
	ModeTilt
	ModeShuttle
)

var modeNames = []string{
	"Off", "Static", "Single On", "Single Off", "Glittering", "Falling",
	"Colourful", "Breath", "Spectrum", "Outward", "Scrolling", "Rolling",
	"Rotating", "Explode", "Launch", "Ripples", "Flowing", "Pulsating",
	"Tilt", "Shuttle",
}

func LightingModes() []string {
	out := make([]string, len(modeNames))
	copy(out, modeNames)
	return out
}

func buildLightingPayload(cfg protocol.LightingConfig) ([reportSize]byte, error) {
	var payload [reportSize]byte
	if cfg.Mode < 0 || cfg.Mode >= len(modeNames) {
		return payload, fmt.Errorf("invalid lighting mode %d (valid range 0-%d)", cfg.Mode, len(modeNames)-1)
	}
	if cfg.Brightness > 5 {
		return payload, fmt.Errorf("invalid brightness %d (valid range 0-5)", cfg.Brightness)
	}
	if cfg.Speed > 5 {
		return payload, fmt.Errorf("invalid speed %d (valid range 0-5)", cfg.Speed)
	}
	if cfg.Direction > 1 {
		return payload, fmt.Errorf("invalid direction %d (valid range 0-1)", cfg.Direction)
	}

	payload[0] = byte(cfg.Mode)
	if cfg.Mode != int(ModeOff) {
		payload[1] = cfg.R
		payload[2] = cfg.G
		payload[3] = cfg.B
		if cfg.Colorful {
			payload[8] = 1
		}
		payload[9] = cfg.Brightness
		payload[10] = cfg.Speed
		payload[11] = cfg.Direction
	}
	// Lighting packets use this command-specific marker in the currently
	// hardware-tested upstream implementation. Do not generalise trailer byte
	// order across other command families.
	payload[14] = 0x55
	payload[15] = 0xAA
	return payload, nil
}

func (d *Device) SetLighting(ctx context.Context, cfg protocol.LightingConfig) error {
	payload, err := buildLightingPayload(cfg)
	if err != nil {
		return err
	}
	if err := ctx.Err(); err != nil {
		return err
	}

	if err := d.beginTransaction(ctx); err != nil {
		return fmt.Errorf("begin: %w", err)
	}
	if err := d.lightingInit(ctx); err != nil {
		return fmt.Errorf("lighting init: %w", err)
	}
	if err := d.sendCommand(ctx, payload, false); err != nil {
		return fmt.Errorf("lighting data: %w", err)
	}
	if err := d.applyTransaction(ctx); err != nil {
		return fmt.Errorf("apply: %w", err)
	}
	if err := d.finalizeTransaction(ctx); err != nil {
		return fmt.Errorf("finalize: %w", err)
	}
	return nil
}
