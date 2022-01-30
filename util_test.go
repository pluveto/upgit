package main

import (
	"testing"
)

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
