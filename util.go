package main

import (
	"bytes"
	"path/filepath"

	"github.com/pelletier/go-toml/v2"
	"github.com/pluveto/upgit/lib/xlog"
	"github.com/pluveto/upgit/lib/xpath"
)

func MustGetApplicationPath(append string) string {
	path, err := xpath.GetApplicationPath()
	if err != nil {
		xlog.AbortErr(err)
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

func RemoveJsoncComments(data []byte) []byte {
	var buf bytes.Buffer
	var inQuote bool
	var inComment bool
	for _, b := range data {
		if b == '"' {
			inQuote = !inQuote
		}
		if inQuote {
			buf.WriteByte(b)
			continue
		}
		if b == '/' {
			inComment = true
		}
		if b == '\n' {
			inComment = false
		}
		if inComment {
			continue
		}
		buf.WriteByte(b)
	}
	return buf.Bytes()
}
