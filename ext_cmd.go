package main

import (
	"errors"
	"fmt"
	"io/ioutil"
	"os"
	"path"
	"strings"

	"github.com/alexflint/go-arg"
	"github.com/pluveto/upgit/lib/xext"
	"github.com/pluveto/upgit/lib/xgithub"
	"github.com/pluveto/upgit/lib/xlog"
	"github.com/pluveto/upgit/lib/xpath"
)

type ExtListCmd struct {
}

type ExtListLocalCmd struct {
}

type ExtAddCmd struct {
	Name string `arg:"positional"`
}

type ExtRemoveCmd struct {
	Name string `arg:"positional"`
}

type ExtCmd struct {
	ListLocal *ExtListCmd   `arg:"subcommand:listlocal,subcommand:lsmy,subcommand:my"`
	List      *ExtListCmd   `arg:"subcommand:list,subcommand:ls"`
	Add       *ExtAddCmd    `arg:"subcommand:add,subcommand:install"`
	Remove    *ExtRemoveCmd `arg:"subcommand:remove,subcommand:rm"`
}

type ExtArgs struct {
	Ext *ExtCmd `arg:"subcommand:ext"`
}

var extArgs ExtArgs

func autoFixExtName(extName string) string {
	if !strings.Contains(extName, ".") {
		extName += ".jsonc"
	}
	return extName
}

func extSubcommand() {
	err := arg.Parse(&extArgs)
	if err != nil || extArgs.Ext == nil {
		os.Stderr.WriteString("Error: " + err.Error() + "\n")
		printExtHelp()
		return
	}
	extDir := xpath.MustGetApplicationPath("extensions")

	switch {
	case extArgs.Ext.List != nil:
		ls, err := xgithub.ListFolder("pluveto/upgit", "/extensions")
		if err != nil {
			xlog.AbortErr(err)
		}
		fmt.Println("All Extensions:")
		for i, v := range ls {
			fmt.Printf("%d. %s\n", i, v.Name)
		}
		os.Exit(0)

	case extArgs.Ext.Add != nil:
		extName := extArgs.Ext.Add.Name
		if len(extName) == 0 {
			xlog.AbortErr(errors.New("extension name is required"))
		}
		extName = autoFixExtName(extName)
		buf, err := xgithub.GetFile("pluveto/upgit", "master", "/extensions/"+extName)
		if err != nil {
			xlog.AbortErr(errors.New("extension " + extName + " not found or network error: " + err.Error()))
		}
		// save buf
		file, err := os.Create(path.Join(extDir, extName))
		defer file.Close()
		xlog.AbortErr(err)
		_, err = file.Write(buf)
		xlog.AbortErr(err)
		fmt.Println("Extension installed:", extName)
		os.Exit(0)

	case extArgs.Ext.Remove != nil:
		extName := extArgs.Ext.Remove.Name
		if len(extName) == 0 {
			xlog.AbortErr(errors.New("extension name is required"))
		}
		extName = autoFixExtName(extName)
		extPath := path.Join(extDir, extName)
		err := os.Remove(extPath)
		xlog.AbortErr(err)
		fmt.Println("Extension removed:", extName)
		os.Exit(0)

	case extArgs.Ext.ListLocal != nil:
		files, err := ioutil.ReadDir(extDir)
		xlog.AbortErr(err)
		fmt.Println("Installed Extensions:")
		for i, v := range files {
			uploaderDef, err := xext.GetExtDefinition(extDir, v.Name())
			xlog.AbortErr(err)
			uploaderDef.DisplaySimple(fmt.Sprintf("%2d ", i), "\n")
		}
		os.Exit(0)

	}

	os.Stderr.WriteString("Unknown subcommand\n")
	printExtHelp()
	os.Exit(0)
}

func printExtHelp() {
	os.Stdout.WriteString("Usage: upgit ext [list|my|add|remove]\n")
}
