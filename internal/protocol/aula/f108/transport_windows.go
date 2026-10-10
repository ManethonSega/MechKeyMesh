//go:build windows

// Portions adapted from github.com/parsiya/f108-pro (MIT License).
// Copyright (c) 2026 Parsia "CryptoGangsta" Hakimian.

package f108

import (
	"errors"
	"fmt"
	"strings"
	"syscall"
	"unsafe"

	"github.com/ManethonSega/MechKeyMesh/internal/protocol"
)

var (
	modHID       = syscall.NewLazyDLL("hid.dll")
	modSetupAPI  = syscall.NewLazyDLL("setupapi.dll")
	modKernel32  = syscall.NewLazyDLL("kernel32.dll")

	procHidDGetHidGuid                  = modHID.NewProc("HidD_GetHidGuid")
	procHidDGetAttributes               = modHID.NewProc("HidD_GetAttributes")
	procHidDSetFeature                  = modHID.NewProc("HidD_SetFeature")
	procHidDGetFeature                  = modHID.NewProc("HidD_GetFeature")
	procHidDGetPreparsedData            = modHID.NewProc("HidD_GetPreparsedData")
	procHidDFreePreparsedData           = modHID.NewProc("HidD_FreePreparsedData")
	procHidPGetCaps                     = modHID.NewProc("HidP_GetCaps")
	procSetupDiGetClassDevsW            = modSetupAPI.NewProc("SetupDiGetClassDevsW")
	procSetupDiEnumDeviceInterfaces     = modSetupAPI.NewProc("SetupDiEnumDeviceInterfaces")
	procSetupDiGetDeviceInterfaceDetailW = modSetupAPI.NewProc("SetupDiGetDeviceInterfaceDetailW")
	procSetupDiDestroyDeviceInfoList    = modSetupAPI.NewProc("SetupDiDestroyDeviceInfoList")
	procCreateFileW                     = modKernel32.NewProc("CreateFileW")
	procCloseHandle                     = modKernel32.NewProc("CloseHandle")
)

const (
	digcfPresent            = 0x02
	digcfDeviceInterface    = 0x10
	genericRead             = 0x80000000
	genericWrite            = 0x40000000
	fileShareRead           = 0x01
	fileShareWrite          = 0x02
	openExisting            = 3
	invalidHandleValue      = ^syscall.Handle(0)
	configurationUsagePage  = 0xFF13
)

type guid struct {
	Data1 uint32
	Data2 uint16
	Data3 uint16
	Data4 [8]byte
}

type spDeviceInterfaceData struct {
	CbSize    uint32
	ClassGUID guid
	Flags     uint32
	Reserved  uintptr
}

type hidAttributes struct {
	Size          uint32
	VendorID      uint16
	ProductID     uint16
	VersionNumber uint16
}

type hidpCaps struct {
	Usage, UsagePage, InputReportByteLength, OutputReportByteLength, FeatureReportByteLength uint16
	Reserved [17]uint16
	NumberLinkCollectionNodes, NumberInputButtonCaps, NumberInputValueCaps, NumberInputDataIndices uint16
	NumberOutputButtonCaps, NumberOutputValueCaps, NumberOutputDataIndices uint16
	NumberFeatureButtonCaps, NumberFeatureValueCaps, NumberFeatureDataIndices uint16
}

type winTransport struct {
	featureHandle syscall.Handle
}

func openTransport() (transport, error) {
	path, err := findHIDDevice(VendorID, ProductID, configurationUsagePage)
	if err != nil {
		return nil, err
	}
	h, err := openHIDHandle(path)
	if err != nil {
		return nil, fmt.Errorf("open F108 configuration interface: %w", err)
	}
	return &winTransport{featureHandle: h}, nil
}

func (t *winTransport) SetFeatureReport(data [reportSize]byte) error {
	buf := make([]byte, reportSize+1)
	copy(buf[1:], data[:])
	r, _, callErr := procHidDSetFeature.Call(
		uintptr(t.featureHandle),
		uintptr(unsafe.Pointer(&buf[0])),
		uintptr(len(buf)),
	)
	if r == 0 {
		return fmt.Errorf("HidD_SetFeature: %w", callErr)
	}
	return nil
}

func (t *winTransport) GetFeatureReport() ([reportSize]byte, error) {
	buf := make([]byte, reportSize+1)
	r, _, callErr := procHidDGetFeature.Call(
		uintptr(t.featureHandle),
		uintptr(unsafe.Pointer(&buf[0])),
		uintptr(len(buf)),
	)
	if r == 0 {
		return [reportSize]byte{}, fmt.Errorf("HidD_GetFeature: %w", callErr)
	}
	var out [reportSize]byte
	copy(out[:], buf[1:])
	return out, nil
}

func (t *winTransport) Close() error {
	if t.featureHandle == 0 || t.featureHandle == invalidHandleValue {
		return nil
	}
	r, _, callErr := procCloseHandle.Call(uintptr(t.featureHandle))
	t.featureHandle = invalidHandleValue
	if r == 0 {
		return fmt.Errorf("CloseHandle: %w", callErr)
	}
	return nil
}

func findHIDDevice(vid, pid, usagePage uint16) (string, error) {
	var hidGUID guid
	procHidDGetHidGuid.Call(uintptr(unsafe.Pointer(&hidGUID)))

	devInfo, _, _ := procSetupDiGetClassDevsW.Call(
		uintptr(unsafe.Pointer(&hidGUID)),
		0,
		0,
		digcfPresent|digcfDeviceInterface,
	)
	if syscall.Handle(devInfo) == invalidHandleValue {
		return "", fmt.Errorf("SetupDiGetClassDevsW failed")
	}
	defer procSetupDiDestroyDeviceInfoList.Call(devInfo)

	for i := uint32(0); ; i++ {
		var iface spDeviceInterfaceData
		iface.CbSize = uint32(unsafe.Sizeof(iface))
		r, _, _ := procSetupDiEnumDeviceInterfaces.Call(
			devInfo,
			0,
			uintptr(unsafe.Pointer(&hidGUID)),
			uintptr(i),
			uintptr(unsafe.Pointer(&iface)),
		)
		if r == 0 {
			break
		}

		path, err := getDeviceInterfaceDetail(devInfo, &iface)
		if err != nil {
			continue
		}
		h, err := openHIDHandle(path)
		if err != nil {
			continue
		}

		attrsOK, capsOK := false, false
		var attrs hidAttributes
		attrs.Size = uint32(unsafe.Sizeof(attrs))
		if r, _, _ := procHidDGetAttributes.Call(uintptr(h), uintptr(unsafe.Pointer(&attrs))); r != 0 &&
			attrs.VendorID == vid && attrs.ProductID == pid {
			attrsOK = true
		}

		if attrsOK {
			var pp uintptr
			if r, _, _ := procHidDGetPreparsedData.Call(uintptr(h), uintptr(unsafe.Pointer(&pp))); r != 0 {
				var caps hidpCaps
				if status, _, _ := procHidPGetCaps.Call(pp, uintptr(unsafe.Pointer(&caps))); status != 0 &&
					caps.UsagePage == usagePage {
					capsOK = true
				}
				procHidDFreePreparsedData.Call(pp)
			}
		}
		procCloseHandle.Call(uintptr(h))
		if attrsOK && capsOK {
			return path, nil
		}
	}
	return "", protocol.ErrNotFound
}

func getDeviceInterfaceDetail(devInfo uintptr, iface *spDeviceInterfaceData) (string, error) {
	var required uint32
	procSetupDiGetDeviceInterfaceDetailW.Call(
		devInfo,
		uintptr(unsafe.Pointer(iface)),
		0,
		0,
		uintptr(unsafe.Pointer(&required)),
		0,
	)
	if required == 0 {
		return "", errors.New("device interface detail size query failed")
	}

	buf := make([]byte, required)
	cbSize := uint32(8)
	if unsafe.Sizeof(uintptr(0)) == 4 {
		cbSize = 6
	}
	*(*uint32)(unsafe.Pointer(&buf[0])) = cbSize

	r, _, callErr := procSetupDiGetDeviceInterfaceDetailW.Call(
		devInfo,
		uintptr(unsafe.Pointer(iface)),
		uintptr(unsafe.Pointer(&buf[0])),
		uintptr(required),
		0,
		0,
	)
	if r == 0 {
		return "", fmt.Errorf("SetupDiGetDeviceInterfaceDetailW: %w", callErr)
	}

	pathBytes := buf[4:]
	path := syscall.UTF16ToString((*[1 << 15]uint16)(unsafe.Pointer(&pathBytes[0]))[:required/2-2])
	path = strings.TrimSpace(path)
	if path == "" {
		return "", errors.New("empty HID path")
	}
	return path, nil
}

func openHIDHandle(path string) (syscall.Handle, error) {
	p, err := syscall.UTF16PtrFromString(path)
	if err != nil {
		return invalidHandleValue, err
	}
	r, _, callErr := procCreateFileW.Call(
		uintptr(unsafe.Pointer(p)),
		genericRead|genericWrite,
		fileShareRead|fileShareWrite,
		0,
		openExisting,
		0,
		0,
	)
	h := syscall.Handle(r)
	if h == invalidHandleValue {
		return h, fmt.Errorf("CreateFileW: %w", callErr)
	}
	return h, nil
}
