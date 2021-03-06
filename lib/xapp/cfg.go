package xapp

type Config struct {
	DefaultUploader string            `toml:"default_uploader,omitempty"`
	Rename          string            `toml:"rename,omitempty"`
	Replacements    map[string]string `toml:"replacements,omitempty"`
	OutputFormats   map[string]string `toml:"output_formats,omitempty"`
}

var AppCfg Config
