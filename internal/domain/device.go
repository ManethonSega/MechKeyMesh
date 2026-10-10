package domain

import "time"

// DeviceState is the user-visible lifecycle state of the active keyboard.
type DeviceState string

const (
	StateDisconnected DeviceState = "disconnected"
	StateDetecting    DeviceState = "detecting"
	StateOpening      DeviceState = "opening"
	StateReady        DeviceState = "ready"
	StateBusy         DeviceState = "busy"
	StateRecovery     DeviceState = "recovery"
	StateUnsupported  DeviceState = "unsupported"
	StateError        DeviceState = "error"
)

// DeviceInfo is stable identity/capability information supplied by a driver.
type DeviceInfo struct {
	DriverID       string   `json:"driverId"`
	Name           string   `json:"name"`
	Vendor         string   `json:"vendor"`
	Connection     string   `json:"connection"`
	VendorID       uint16   `json:"vendorId"`
	ProductID      uint16   `json:"productId"`
	Capabilities   []string `json:"capabilities"`
	Firmware       string   `json:"firmware,omitempty"`
	BatteryPercent *int     `json:"batteryPercent,omitempty"`
}

// DeviceStatus is a snapshot safe to expose to the frontend.
type DeviceStatus struct {
	State       DeviceState `json:"state"`
	Info        DeviceInfo  `json:"info"`
	LastError   string      `json:"lastError,omitempty"`
	LastChanged time.Time   `json:"lastChanged"`
}
