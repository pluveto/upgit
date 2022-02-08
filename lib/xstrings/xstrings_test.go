package xstrings

import (
	"reflect"
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

func TestRemoveFmtUnderscore(t *testing.T) {
	type args struct {
		in string
	}
	tests := []struct {
		name    string
		args    args
		wantOut string
	}{
		{"1", args{"a{b}c"}, "a{b}c"},
		{"2", args{"_a_{_b_}_c_"}, "_a_{b}_c_"},
		{"3", args{"{_b_/_c_}_c_{d}{_e}{f_}"}, "{b/c}_c_{d}{e}{f}"},
		{"4", args{"{{a_b}}{{{{"}, "{{ab}}{{{{"},
		{"5", args{"upgit_20220130_{fname_hash8}.jpg"}, "upgit_20220130_{fnamehash8}.jpg"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if gotOut := RemoveFmtUnderscore(tt.args.in); gotOut != tt.wantOut {
				t.Errorf("RemoveFmtUnderscore() = %v, want %v", gotOut, tt.wantOut)
			}
		})
	}
}

func TestRemoveJsoncComments(t *testing.T) {
	type args struct {
		data []byte
	}
	tests := []struct {
		name string
		args args
		want []byte
	}{
		{"1", args{
			[]byte(
				`
a//bcde//
//c
//
a
`,
			),
		},
			[]byte(
				`
a


a
`,
			),
		},
		{"1", args{
			[]byte(
				`
{
	url: "http://www.example.com" // url
}
`,
			),
		},
			[]byte(
				`
{
	url: "http://www.example.com" 
}
`,
			),
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := RemoveJsoncComments(tt.args.data); !reflect.DeepEqual(got, tt.want) {
				t.Errorf("RemoveJsoncComments() = %v, want %v", string(got), string(tt.want))
			}
		})
	}
}
