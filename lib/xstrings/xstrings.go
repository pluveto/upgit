package xstrings

import (
	"bytes"
	"regexp"
	"strings"
)

func VariableReplace(s, delimiterLeft, delimiterRight string, dict map[string]string) string {
	ret := s
	for k, v := range dict {
		ret = strings.Replace(ret, delimiterLeft+k+delimiterRight, v, -1)
	}
	return ret
}

func VariableReplaceFunc(s, delimiterLeft, delimiterRight string, dictFunc func(string) *string) *string {
	ret := s
	r := regexp.MustCompile(regexp.QuoteMeta(delimiterLeft) + "(.*?)" + regexp.QuoteMeta(delimiterRight))
	for _, v := range r.FindAllStringSubmatch(ret, -1) {
		val := dictFunc(v[1])
		if nil == val {
			return nil
		}
		ret = strings.Replace(ret, v[0], *val, -1)
	}
	return &ret
}

func ValueOrDefault(try, default_ string) string {
	if try == "" {
		return default_
	}
	return try
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
