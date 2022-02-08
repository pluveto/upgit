package xext

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"path/filepath"

	"github.com/fatih/color"
	"github.com/pluveto/upgit/lib/xstrings"
)

type ExtDef struct {
	Meta struct {
		Id          string `mapstructure:"id"`
		Name        string `mapstructure:"name"`
		Author      string `mapstructure:"author"`
		Description string `mapstructure:"description"`
		Type        string `mapstructure:"type"`
		Version     string `mapstructure:"version"`
		Repository  string `mapstructure:"repository"`
	} `mapstructure:"meta"`
}

func (e ExtDef) GetId() string {
	return e.Meta.Id
}

func (e ExtDef) DisplaySimple(prefix, suffix string) {
	// Gitub Uploader (id: github) v1.0.0 - Description
	fmt.Print(prefix)
	fmt.Printf("%-32s", color.CyanString(e.Meta.Name))
	fmt.Printf("id: %-16s", color.YellowString(e.Meta.Id))
	if len(e.Meta.Version) > 0 {
		fmt.Printf(" v%-8s", e.Meta.Version)
	}
	if len(e.Meta.Description) > 0 {
		fmt.Print("- " + e.Meta.Description)
	}
	fmt.Print(suffix)
}

func GetExtDefinitionInterface(extDir, fname string) (map[string]interface{}, error) {
	jsonBytes, err := ioutil.ReadFile(filepath.Join(extDir, fname))
	if err != nil {
		return nil, err
	}
	jsonBytes = xstrings.RemoveJsoncComments(jsonBytes)
	var uploaderDef map[string]interface{}
	err = json.Unmarshal(jsonBytes, &uploaderDef)
	return uploaderDef, err
}

func GetExtDefinition(extDir, fname string) (ExtDef, error) {
	jsonBytes, err := ioutil.ReadFile(filepath.Join(extDir, fname))
	if err != nil {
		return ExtDef{}, err
	}
	jsonBytes = xstrings.RemoveJsoncComments(jsonBytes)
	var uploaderDef ExtDef
	err = json.Unmarshal(jsonBytes, &uploaderDef)
	return uploaderDef, err
}
