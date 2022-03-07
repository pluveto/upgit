package xapp

import (
	"crypto/md5"
	"fmt"
	"io/ioutil"
	"path/filepath"
	"strings"
	"time"

	"github.com/mitchellh/mapstructure"
	"github.com/pelletier/go-toml/v2"
	"github.com/pluveto/upgit/lib/xpath"
)

const UserAgent = "UPGIT/0.2"
const DefaultBranch = "master"
const ClipboardPlaceholder = ":clipboard"

var MaxUploadSize = int64(5 * 1024 * 1024)
var ConfigFilePath string

func Rename(path string, time time.Time) (ret string) {

	base := xpath.Basename(path)
	ext := filepath.Ext(path)
	md5HashStr := fmt.Sprintf("%x", md5.Sum([]byte(base)))
	r := strings.NewReplacer(
		"{year}", time.Format("2006"),
		"{month}", time.Format("01"),
		"{day}", time.Format("02"),
		"{hour}", time.Format("15"),
		"{minute}", time.Format("04"),
		"{second}", time.Format("05"),
		"{unixts}", fmt.Sprint(time.Unix()),
		"{unixtsms}", fmt.Sprint(time.UnixMicro()),
		"{ext}", ext,
		"{fullname}", base+ext,
		"{filename}", base,
		"{fname}", base,
		"{filenamehash}", md5HashStr,
		"{fnamehash}", md5HashStr,
		"{fnamehash4}", md5HashStr[:4],
		"{fnamehash8}", md5HashStr[:8],
	)
	ret = r.Replace(AppCfg.Rename)
	return
}
func ReplaceUrl(path string) (ret string) {
	var rules []string
	for k, v := range AppCfg.Replacements {
		rules = append(rules, k, v)
	}
	r := strings.NewReplacer(rules...)
	ret = r.Replace(path)
	return
}

func LoadUploaderConfig[T any](uploaderId string) (ret T, err error) {
	var mCfg map[string]interface{}
	bytes, err := ioutil.ReadFile(ConfigFilePath)
	if err != nil {
		return
	}
	err = toml.Unmarshal(bytes, &mCfg)
	if err != nil {
		return
	}
	cfgMap := mCfg["uploaders"].(map[string]interface{})[uploaderId]
	var cfg_ T
	mapstructure.Decode(cfgMap, &cfg_)
	ret = cfg_
	return
}
