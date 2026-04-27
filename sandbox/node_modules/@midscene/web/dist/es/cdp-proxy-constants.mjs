import { tmpdir } from "node:os";
import { join } from "node:path";
const PROXY_ENDPOINT_FILE = join(tmpdir(), 'midscene-cdp-proxy-endpoint');
const PROXY_PID_FILE = join(tmpdir(), 'midscene-cdp-proxy-pid');
const PROXY_UPSTREAM_FILE = join(tmpdir(), 'midscene-cdp-proxy-upstream');
const TARGET_ID_FILE = join(tmpdir(), 'midscene-cdp-target-id');
export { PROXY_ENDPOINT_FILE, PROXY_PID_FILE, PROXY_UPSTREAM_FILE, TARGET_ID_FILE };

//# sourceMappingURL=cdp-proxy-constants.mjs.map