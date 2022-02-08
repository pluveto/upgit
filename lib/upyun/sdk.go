package upyun

import (
	"bytes"
	"crypto/md5"
	"errors"
	"fmt"
	"io"
	"math"
	"net"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"
)

type UpYun struct {
	httpClient *http.Client
	trans      *http.Transport
	bucketName string
	userName   string
	passWord   string
	apiDomain  string
	contentMd5 string
	fileSecret string
	tmpHeaders map[string]string

	TimeOut int
	Debug   bool
}

/**
 * 初始化 UpYun 存储接口
 * @param bucketName 空间名称
 * @param userName 操作员名称
 * @param passWord 密码
 * return UpYun object
 */
func NewUpYun(bucketName, userName, passWord string) *UpYun {
	u := new(UpYun)
	u.TimeOut = 300
	u.httpClient = &http.Client{}
	u.httpClient.Transport = &http.Transport{Dial: timeoutDialer(u.TimeOut)}
	u.bucketName = bucketName
	u.userName = userName
	u.passWord = StringMd5(passWord)
	u.apiDomain = "v0.api.upyun.com"
	u.Debug = false
	return u
}

func (u *UpYun) Version() string {
	return "1.0.1"
}

/**
* 切换 API 接口的域名
* @param domain {
默认 v0.api.upyun.com 自动识别,
    v1.api.upyun.com 电信,
    v2.api.upyun.com 联通,
    v3.api.upyun.com 移动
}
* return 无
*/
func (u *UpYun) SetApiDomain(domain string) {
	u.apiDomain = domain
}

/**
 * 设置连接超时时间
 * @param time 秒
 * return 无
 */
func (u *UpYun) SetTimeout(time int) {
	u.TimeOut = time
	u.httpClient.Transport = &http.Transport{Dial: timeoutDialer(u.TimeOut)}
}

/**
 * 设置待上传文件的 Content-MD5 值（如又拍云服务端收到的文件MD5值与用户设置的不一致，
 * 将回报 406 Not Acceptable 错误）
 * @param str （文件 MD5 校验码）
 * return 无
 */
func (u *UpYun) SetContentMD5(str string) {
	u.contentMd5 = str
}

/**
 * 连接签名方法
 * @param method 请求方式 {GET, POST, PUT, DELETE}
 * return 签名字符串
 */
func (u *UpYun) sign(method, uri, date string, length int64) string {
	var bufSign bytes.Buffer
	bufSign.WriteString(method)
	bufSign.WriteString("&")
	bufSign.WriteString(uri)
	bufSign.WriteString("&")
	bufSign.WriteString(date)
	bufSign.WriteString("&")
	bufSign.WriteString(strconv.FormatInt(length, 10))
	bufSign.WriteString("&")
	bufSign.WriteString(u.passWord)

	var buf bytes.Buffer
	buf.WriteString("UpYun ")
	buf.WriteString(u.userName)
	buf.WriteString(":")
	buf.WriteString(StringMd5(bufSign.String()))
	return buf.String()
}

/**
 * 连接处理逻辑
 * @param method 请求方式 {GET, POST, PUT, DELETE}
 * @param uri 请求地址
 * @param inFile 如果是POST上传文件，传递文件IO数据流
 * @param outFile 如果是GET下载文件，可传递文件IO数据流，这种情况函数也返回""
 * return 请求返回字符串，失败返回""(打开debug状态下遇到错误将中止程序执行)
 */
func (u *UpYun) httpAction(method, uri string, headers map[string]string,
	inFile, outFile *os.File) (string, error) {
	uri = "/" + u.bucketName + uri
	url := "http://" + u.apiDomain + uri
	req, err := http.NewRequest(method, url, nil)
	if err != nil {
		if u.Debug {
			fmt.Println(err)
			panic("http.NewRequest failed: " + err.Error())
		}
		return "", err
	}

	for k, v := range headers {
		req.Header.Add(k, v)
	}

	length := FileSize(inFile)
	if u.Debug {
		fmt.Println("inFileSize: ", length)
	}

	if method == "PUT" || method == "POST" {
		method = "POST"
		if inFile != nil {
			if u.contentMd5 != "" {
				req.Header.Add("Content-MD5", u.contentMd5)
				u.contentMd5 = ""
			}
			if u.fileSecret != "" {
				req.Header.Add("Content-Secret", u.fileSecret)
				u.fileSecret = ""
			}
			req.Header.Add("Content-Length", strconv.FormatInt(length, 10))
			req.Body = inFile
			req.ContentLength = length
		}
	}
	req.Method = method

	date := time.Now().UTC().Format(time.RFC1123)
	req.Header.Add("Date", date)
	req.Header.Add("Authorization", u.sign(method, uri, date, length))

	if method == "HEAD" {
		req.Body = nil
	}

	if u.Debug {
		fmt.Println(req)
	}
	resp, err := u.httpClient.Do(req)
	if err != nil {
		if u.Debug {
			fmt.Println(resp.Status, err)
			panic("httpClient.Do failed: " + resp.Status + err.Error())
		}
		return "", err
	}

	rc := resp.StatusCode
	if rc == 200 {
		u.tmpHeaders = make(map[string]string)
		for k, v := range resp.Header {
			if strings.Contains(k, "x-upyun") {
				u.tmpHeaders[k] = v[0]
			}
		}

		if method == "GET" && outFile != nil {
			_, err := io.Copy(outFile, resp.Body)
			if err != nil {
				if u.Debug {
					fmt.Printf("%v %v\n", rc, err)
					panic("write output file failed: ")
				}
				return "", err
			}
			return "", nil
		}

		buf := bytes.NewBuffer(make([]byte, 0, 8192))
		buf.ReadFrom(resp.Body)
		return buf.String(), nil
	}

	return "", errors.New(resp.Status)
}

/**
 * 获取总体空间的占用信息
 * return 空间占用量，失败返回0.0
 */
func (u *UpYun) GetBucketUsage() (float64, error) {
	return u.GetFolderUsage("/")
}

/**
 * 获取某个子目录的占用信息
 * @param $path 目标路径
 * return 空间占用量和error，失败空间占用量返回0.0
 */
func (u *UpYun) GetFolderUsage(path string) (float64, error) {
	r, err := u.httpAction("GET", path+"?usage", nil, nil, nil)
	if err != nil {
		return 0.0, err
	}
	v, _ := strconv.ParseFloat(r, 64)
	return v, nil
}

/**
 * 设置待上传文件的 访问密钥（注意：仅支持图片空！，设置密钥后，无法根据原文件URL直接访问，需带 URL 后面加上 （缩略图间隔标志符+密钥） 进行访问）
 * 如缩略图间隔标志符为 ! ，密钥为 bac，上传文件路径为 /folder/test.jpg ，那么该图片的对外访问地址为： http://空间域名/folder/test.jpg!bac
 * @param $str （文件 MD5 校验码）
 * return null;
 */
func (u *UpYun) SetFileSecret(str string) {
	u.fileSecret = str
}

/**
 * 上传文件
 * @param filePath 文件路径（包含文件名）
 * @param inFile 文件IO数据流
 * @param autoMkdir 是否自动创建父级目录(最深10级目录)
 * return error
 */
func (u *UpYun) WriteFile(filePath string, inFile *os.File, autoMkdir bool) error {
	var headers map[string]string
	if autoMkdir {
		headers = make(map[string]string)
		headers["Mkdir"] = "true"
		//如果文件超过100kb，等比例压缩质量
		sta, err := inFile.Stat()
		if err != nil {
			return err
		}
		if sta.Size() > 102400 { //大于 100kb
			headers["x-gmkerl-type"] = "fix_scale"
			cof := math.Sqrt(102400) / math.Sqrt(float64(sta.Size())) * 100
			headers["x-gmkerl-value"] = strconv.FormatFloat(cof, 'f', 0, 64) //缩放
			headers["x-gmkerl-quality"] = "50"
		} else if sta.Size() > 102400/2 { //大于50kb
			headers["x-gmkerl-quality"] = "75"
		}
	}
	_, err := u.httpAction("PUT", filePath, headers, inFile, nil)
	return err
}

/**
 * 获取上传文件后的信息（仅图片空间有返回数据）
 * @param key 信息字段名（x-upyun-width、x-upyun-height、x-upyun-frames、x-upyun-file-type）
 * return string or ""
 */
func (u *UpYun) GetWritedFileInfo(key string) string {
	if u.tmpHeaders == nil {
		return ""
	}
	return u.tmpHeaders[strings.ToLower(key)]
}

/**
 * 读取文件
 * @param file 文件路径（包含文件名）
 * @param outFile 可传递文件IO数据流（结果返回true or false）
 * return error
 */
func (u *UpYun) ReadFile(file string, outFile *os.File) error {
	_, err := u.httpAction("GET", file, nil, nil, outFile)
	return err
}

/**
 * 获取文件信息
 * @param file 文件路径（包含文件名）
 * return array('type': file | folder, 'size': file size, 'date': unix time) 或 nil
 */
func (u *UpYun) GetFileInfo(file string) map[string]string {
	_, err := u.httpAction("HEAD", file, nil, nil, nil)
	if err != nil {
		return nil
	}
	if u.tmpHeaders == nil {
		return nil
	}
	m := make(map[string]string)
	if v, ok := u.tmpHeaders["x-upyun-file-type"]; ok {
		m["type"] = v
	}
	if v, ok := u.tmpHeaders["x-upyun-file-size"]; ok {
		m["size"] = v
	}
	if v, ok := u.tmpHeaders["x-upyun-file-date"]; ok {
		m["date"] = v
	}
	return m
}

type DirInfo struct {
	Name string
	Type string
	Size int64
	Time int64
}

/**
 * 读取目录列表
 * @param path 目录路径
 * return DirInfo数组 或 nil
 */
func (u *UpYun) ReadDir(path string) ([]*DirInfo, error) {
	r, err := u.httpAction("GET", path, nil, nil, nil)
	if err != nil {
		return nil, err
	}

	dirs := make([]*DirInfo, 0, 8)
	rs := strings.Split(r, "\n")
	for i := 0; i < len(rs); i++ {
		ri := strings.TrimSpace(rs[i])
		rid := strings.Split(ri, "\t")
		d := new(DirInfo)
		d.Name = rid[0]
		if len(rid) > 3 && rid[3] != "" {
			if rid[1] == "N" {
				d.Type = "file"
			} else {
				d.Type = "folder"
			}
			d.Time, _ = strconv.ParseInt(rid[3], 10, 64)
		}
		if len(rid) > 2 {
			d.Size, _ = strconv.ParseInt(rid[2], 10, 64)
		}
		dirs = append(dirs, d)
	}
	return dirs, nil
}

/**
 * 删除文件
 * @param file 文件路径（包含文件名）
 * return error
 */
func (u *UpYun) DeleteFile(file string) error {
	_, err := u.httpAction("DELETE", file, nil, nil, nil)
	return err
}

/**
 * 创建目录
 * @param path 目录路径
 * @param auto_mkdir=false 是否自动创建父级目录
 * return error
 */
func (u *UpYun) MkDir(path string, autoMkdir bool) error {
	var headers map[string]string
	headers = make(map[string]string)
	headers["Folder"] = "true"
	if autoMkdir {
		headers["Mkdir"] = "true"
	}
	_, err := u.httpAction("PUT", path, headers, nil, nil)
	return err
}

/**
 * 删除目录
 * @param path 目录路径
 * return error
 */
func (u *UpYun) RmDir(dir string) error {
	_, err := u.httpAction("DELETE", dir, nil, nil, nil)
	return err
}

func FileSize(f *os.File) int64 {
	if f == nil {
		return 0
	}
	if fi, err := f.Stat(); err == nil {
		return fi.Size()
	}
	return 0
}

func StringMd5(s string) string {
	h := md5.New()
	io.WriteString(h, s)
	return fmt.Sprintf("%x", h.Sum(nil))
}

func FileMd5(name string) string {
	f, err := os.Open(name)
	if err != nil {
		return ""
	}
	defer f.Close()

	h := md5.New()
	io.Copy(h, f)
	return fmt.Sprintf("%x", h.Sum(nil))
}

func timeoutDialer(timeout int) func(string, string) (net.Conn, error) {
	return func(netw, addr string) (c net.Conn, err error) {
		delta := time.Duration(timeout) * time.Second
		c, err = net.DialTimeout(netw, addr, delta)
		if err != nil {
			return nil, err
		}
		c.SetDeadline(time.Now().Add(delta))
		return c, nil
	}
}
