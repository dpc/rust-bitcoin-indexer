package main

import (
	"context"
	"encoding/json"
	"github.com/gin-gonic/gin"
	"github.com/ybbus/jsonrpc/v3"
	"golang.org/x/net/http2"
	"golang.org/x/net/http2/h2c"
	"io/ioutil"
	"log"
	"net/http"
	"time"
)

var proxyBufferSize = 4096
var httpListenPort = 8098
var httpsConnectingPort = 443

type RPCRequest struct {
	Method  string        `json:"method"`
	Params  []interface{} `json:"params"`
	ID      int           `json:"id"`
	JSONRPC string        `json:"jsonrpc"`
}

func bitcoinHandler(c *gin.Context) {

	jsonData, err := ioutil.ReadAll(c.Request.Body)
	if err != nil {
		// Handle error
	}

	log.Println(string(jsonData))
	rpcrequest := RPCRequest{}
	if err := json.Unmarshal(jsonData, &rpcrequest); err != nil {
		panic(err)
	}
	rpcClient := jsonrpc.NewClient("https://light-greatest-uranium.btc.quiknode.pro")

	response, err := rpcClient.Call(context.Background(), rpcrequest.Method, rpcrequest.Params)

	c.JSON(http.StatusOK, gin.H{
		//"jsonrpc": "2.0",
		"result": response.Result,
		"id":     rpcrequest.ID,
		"error":  nil,
	})
}

func main() {

	r := gin.Default()
	r.UseH2C = true
	r.POST("/", bitcoinHandler)

	srv := &http.Server{
		Addr: ":8098",
		Handler: h2c.NewHandler(
			r,
			&http2.Server{},
		),
		ReadHeaderTimeout: time.Second,
		ReadTimeout:       5 * time.Minute,
		WriteTimeout:      5 * time.Minute,
		MaxHeaderBytes:    8 * 1024, // 8KiB
	}

	if err := srv.ListenAndServe(); err != nil {
		log.Fatalf("HTTP listen and serve: %v", err)
	}

}
