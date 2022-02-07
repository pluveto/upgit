package xstrings

import (
	"testing"
)

func TestVariableReplace(t *testing.T) {
	type args struct {
		s              string
		delimiterLeft  string
		delimiterRight string
		dict           map[string]string
	}
	tests := []struct {
		name string
		args args
		want string
	}{
		{"1", args{"aba${a}aba", "${", "}", map[string]string{"a": "b"}}, "abababa"},
		{"2", args{"$(a)", "$(", ")", map[string]string{"a": "b"}}, "b"},
		{"3", args{"${a.b.c}", "${", "}", map[string]string{"a.b.c": "d"}}, "d"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := VariableReplace(tt.args.s, tt.args.delimiterLeft, tt.args.delimiterRight, tt.args.dict); got != tt.want {
				t.Errorf("VariableReplace() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestVariableReplaceFunc(t *testing.T) {
	type args struct {
		s              string
		delimiterLeft  string
		delimiterRight string
		dictFunc       func(string) *string
	}
	getVal := func(k string) *string {
		m := map[string]string{"a": "b", "a.b.c": "d"}
		v := m[k]
		return &v
	}

	tests := []struct {
		name string
		args args
		want string
	}{
		{"1", args{"aba${a}aba", "${", "}", getVal}, "abababa"},
		{"2", args{"$(a)", "$(", ")", getVal}, "b"},
		{"3", args{"${a.b.c}", "${", "}", getVal}, "d"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := VariableReplaceFunc(tt.args.s, tt.args.delimiterLeft, tt.args.delimiterRight, tt.args.dictFunc); *got != tt.want {
				t.Errorf("VariableReplaceFunc() = %v, want %v", got, tt.want)
			}
		})
	}
}
