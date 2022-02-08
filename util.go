package main

import (
	"github.com/pelletier/go-toml/v2"
)

func MustMarshall(s interface{}) string {
	b, err := toml.Marshal(s)
	if err != nil {
		return ""
	}
	return string(b)
}
