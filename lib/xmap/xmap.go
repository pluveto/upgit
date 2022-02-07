package xmap

import (
	"errors"
	"regexp"
	"strconv"
	"strings"
)

func GetDeep[T any](m map[string]interface{}, path string) (ret T, err error) {
	if m == nil {
		err = errors.New("map is nil")
		return
	}

	if path == "" {
		ret = interface{}(m).(T)
		return
	}

	keys := strings.Split(path, ".")
	for i := 0; i < len(keys); i++ {
		key := keys[i]
		if key == "" {
			continue
		}
		// match xxx[i]           $1    $2
		r := regexp.MustCompile(`^(.*)\[(\d+)\]$`)
		arrIndex := 0
		matches := r.FindAllString(key, -1)
		if len(matches) > 0 {
			key = string(matches[0][1])
			arrIndex, _ = strconv.Atoi(string(matches[0][2]))
		}

		if m == nil {
			err = errors.New("map is nil")
			return
		}

		if v, ok := m[key]; ok {
			switch v.(type) {
			case []interface{}:
				return v.([]interface{})[arrIndex].(T), nil
			case map[string]interface{}:
				m = v.(map[string]interface{})
			default:
				return v.(T), nil
			}
		} else {
			err = errors.New("for path " + path + ", key " + key + " not found")
			return
		}
	}
	return interface{}(m).(T), nil
}
