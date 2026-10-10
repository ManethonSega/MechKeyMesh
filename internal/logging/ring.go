package logging

import (
	"fmt"
	"sync"
	"time"
)

// Entry is one in-memory diagnostic event.
type Entry struct {
	Time    time.Time `json:"time"`
	Level   string    `json:"level"`
	Message string    `json:"message"`
}

// Ring is a bounded in-memory log used by the diagnostics UI.
type Ring struct {
	mu      sync.RWMutex
	entries []Entry
	max     int
}

func NewRing(max int) *Ring {
	if max < 1 {
		max = 1
	}
	return &Ring{max: max, entries: make([]Entry, 0, max)}
}

func (r *Ring) Add(level, format string, args ...any) {
	e := Entry{Time: time.Now(), Level: level, Message: fmt.Sprintf(format, args...)}
	r.mu.Lock()
	defer r.mu.Unlock()
	if len(r.entries) == r.max {
		copy(r.entries, r.entries[1:])
		r.entries[len(r.entries)-1] = e
		return
	}
	r.entries = append(r.entries, e)
}

func (r *Ring) Snapshot() []Entry {
	r.mu.RLock()
	defer r.mu.RUnlock()
	out := make([]Entry, len(r.entries))
	copy(out, r.entries)
	return out
}
