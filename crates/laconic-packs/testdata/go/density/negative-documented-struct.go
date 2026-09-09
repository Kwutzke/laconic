package density

type Config struct {
	Host    string // the SAP gateway, without a scheme
	Port    int    // 0 selects the gateway's default
	Retries int    // attempts after the first, so 0 means try once
}
