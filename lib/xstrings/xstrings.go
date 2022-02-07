package xstrings

import (
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

func EmptyOrDefault(try, default_ string) string {
	if try == "" {
		return default_
	}
	return try
}
