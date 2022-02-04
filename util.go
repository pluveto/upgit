package main

import (
	"os"
	"path/filepath"

	"github.com/pelletier/go-toml/v2"
)

func GetApplicationPath() (path string, err error) {
	exec, err := os.Executable()
	if err != nil {
		return
	}
	path = filepath.Dir(exec)
	return
}

func MustGetApplicationPath(append string) string {
	path, err := GetApplicationPath()
	if err != nil {
		abortErr(err)
	}
	return filepath.Join(path, append)
}

func MustMarshall(s interface{}) string {
	b, err := toml.Marshal(s)
	if err != nil {
		return ""
	}
	return string(b)
}

// RemoveFmtUnderscore {abc_def_} => {abcdef}
func RemoveFmtUnderscore(in string) (out string) {
	out = ""
	offset := 0
	n := len(in)
	replacing := false
	for offset < n {
		r := in[offset]
		switch {
		case r == '{':
			replacing = true
		case r == '}':
			replacing = false
		}
		if !(replacing && r == '_') {
			out += string(r)
		}
		offset++
	}
	return
}
