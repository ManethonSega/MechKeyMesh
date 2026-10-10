package logging

import "testing"

func TestRingBoundsAndOrder(t *testing.T) {
	r := NewRing(2)
	r.Add("info", "one")
	r.Add("info", "two")
	r.Add("info", "three")

	got := r.Snapshot()
	if len(got) != 2 {
		t.Fatalf("len = %d, want 2", len(got))
	}
	if got[0].Message != "two" || got[1].Message != "three" {
		t.Fatalf("unexpected order: %#v", got)
	}
}
