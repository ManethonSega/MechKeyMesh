//go:build !windows

package f108

import "github.com/ManethonSega/MechKeyMesh/internal/protocol"

func openTransport() (transport, error) { return nil, protocol.ErrUnsupportedPlatform }
