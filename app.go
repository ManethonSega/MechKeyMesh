package main

import (
	"context"
	"fmt"
	"time"

	"github.com/ManethonSega/MechKeyMesh/internal/device"
	"github.com/ManethonSega/MechKeyMesh/internal/domain"
	"github.com/ManethonSega/MechKeyMesh/internal/logging"
	"github.com/ManethonSega/MechKeyMesh/internal/protocol"
	"github.com/ManethonSega/MechKeyMesh/internal/protocol/aula/f108"
)

type App struct {
	ctx     context.Context
	cancel  context.CancelFunc
	manager *device.Manager
	log     *logging.Ring
}

func NewApp() *App {
	log := logging.NewRing(300)
	return &App{
		log:     log,
		manager: device.NewManager(f108.Factory{}, log),
	}
}

func (a *App) startup(ctx context.Context) {
	a.ctx, a.cancel = context.WithCancel(ctx)
	a.log.Add("info", "MechKeyMesh v0.1.0-alpha.1 starting")
	a.manager.Start(a.ctx)
}

func (a *App) shutdown(context.Context) {
	if a.cancel != nil {
		a.cancel()
	}
	a.manager.Close()
}

func (a *App) GetStatus() domain.DeviceStatus { return a.manager.Status() }
func (a *App) GetLogs() []logging.Entry       { return a.log.Snapshot() }
func (a *App) GetLightingModes() []string     { return f108.LightingModes() }
func (a *App) Reconnect()                     { a.manager.Reconnect() }

func (a *App) ApplyLighting(cfg protocol.LightingConfig) error {
	ctx, cancel := context.WithTimeout(a.ctx, 4*time.Second)
	defer cancel()

	return a.manager.Submit(ctx, "apply built-in lighting", 3*time.Second,
		func(ctx context.Context, d protocol.Device) error {
			ld, ok := d.(protocol.LightingDevice)
			if !ok {
				return fmt.Errorf("%s does not support onboard lighting", d.Info().Name)
			}
			return ld.SetLighting(ctx, cfg)
		})
}
