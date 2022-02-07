package qcloudcos

import (
	"fmt"
	"net/http"
	"strings"
	"sync"
	"time"
)

// cloneRequest returns a clone of the provided *http.Request. The clone is a
// shallow copy of the struct and its Header map.
func cloneRequest(r *http.Request) *http.Request {
	// shallow copy of the struct
	r2 := new(http.Request)
	*r2 = *r
	// deep copy of the Header
	r2.Header = make(http.Header, len(r.Header))
	for k, s := range r.Header {
		r2.Header[k] = append([]string(nil), s...)
	}
	return r2
}

// AuthorizationTransport 给请求增加 Authorization header
type AuthorizationTransport struct {
	SecretID     string
	SecretKey    string
	SessionToken string
	rwLocker     sync.RWMutex
	// 签名多久过期
	Expire    time.Duration
	Transport http.RoundTripper
}

// SetCredential update the SecretID(ak), SercretKey(sk), sessiontoken
func (t *AuthorizationTransport) SetCredential(ak, sk, token string) {
	t.rwLocker.Lock()
	defer t.rwLocker.Unlock()
	t.SecretID = ak
	t.SecretKey = sk
	t.SessionToken = token
}

// GetCredential get the ak, sk, token
func (t *AuthorizationTransport) GetCredential() (string, string, string) {
	t.rwLocker.RLock()
	defer t.rwLocker.RUnlock()
	return t.SecretID, t.SecretKey, t.SessionToken
}

// RoundTrip implements the RoundTripper interface.
func (t *AuthorizationTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	// req = cloneRequest(req) // per RoundTrip contract

	ak, sk, token := t.GetCredential()
	if strings.HasPrefix(ak, " ") || strings.HasSuffix(ak, " ") {
		return nil, fmt.Errorf("SecretID is invalid")
	}
	if strings.HasPrefix(sk, " ") || strings.HasSuffix(sk, " ") {
		return nil, fmt.Errorf("SecretKey is invalid")
	}

	// 增加 Authorization header
	authTime := NewAuthTime(defaultAuthExpire)
	AddAuthorizationHeader(ak, sk, token, req, authTime)

	resp, err := t.transport().RoundTrip(req)
	return resp, err
}

func (t *AuthorizationTransport) transport() http.RoundTripper {
	if t.Transport != nil {
		return t.Transport
	}
	return http.DefaultTransport
}
