package f108

type transport interface {
	SetFeatureReport([reportSize]byte) error
	GetFeatureReport() ([reportSize]byte, error)
	Close() error
}
