package xmap

import (
	"errors"
	"strings"
)

func GetDeep[T any](m map[string]interface{}, path string) (ret T, err error) {
	if m == nil {
		err = errors.New("map is nil")
		return
	}

	if path == "" {
		return
	}

	keys := strings.Split(path, ".")
	for i := 0; i < len(keys); i++ {
		key := keys[i]
		if key == "" {
			continue
		}

		if m == nil {
			err = errors.New("map is nil")
			return
		}

		if v, ok := m[key]; ok {
			switch v.(type) {
			case map[string]interface{}:
				m = v.(map[string]interface{})
			default:
				return v.(T), nil
			}
		} else {
			err = errors.New("key not found")
			return
		}
	}
	return
}
