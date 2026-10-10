package f108

import (
	"testing"

	"github.com/ManethonSega/MechKeyMesh/internal/protocol"
)

func TestLightingPayload(t *testing.T) {
	p, err := buildLightingPayload(protocol.LightingConfig{
		Mode: int(ModeBreath), R: 1, G: 2, B: 3,
		Brightness: 5, Speed: 3, Direction: 1, Colorful: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	checks := map[int]byte{0: 7, 1: 1, 2: 2, 3: 3, 8: 1, 9: 5, 10: 3, 11: 1, 14: 0x55, 15: 0xAA}
	for pos, want := range checks {
		if p[pos] != want {
			t.Fatalf("payload[%d] = 0x%02X, want 0x%02X", pos, p[pos], want)
		}
	}
}

func TestLightingValidation(t *testing.T) {
	tests := []protocol.LightingConfig{
		{Mode: -1},
		{Mode: 20},
		{Mode: 1, Brightness: 6},
		{Mode: 1, Speed: 6},
		{Mode: 1, Direction: 2},
	}
	for _, cfg := range tests {
		if _, err := buildLightingPayload(cfg); err == nil {
			t.Fatalf("expected validation error for %#v", cfg)
		}
	}
}

func TestOffPayloadDoesNotLeakParameters(t *testing.T) {
	p, err := buildLightingPayload(protocol.LightingConfig{Mode: 0, R: 255, G: 255, B: 255, Brightness: 5, Speed: 5, Direction: 1, Colorful: true})
	if err != nil {
		t.Fatal(err)
	}
	for _, pos := range []int{1, 2, 3, 8, 9, 10, 11} {
		if p[pos] != 0 {
			t.Fatalf("off payload[%d] = %d, want 0", pos, p[pos])
		}
	}
}
