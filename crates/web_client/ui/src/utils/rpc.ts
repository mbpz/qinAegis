// Shared RPC helper — single source of truth
export async function rpcCall(method: string, params: object = {}): Promise<any> {
  const paramsStr = encodeURIComponent(JSON.stringify(params));
  const url = 'app://localhost/invoke?method=' + encodeURIComponent(method) + '&params=' + paramsStr;
  const resp = await fetch(url);
  const text = await resp.text();
  return JSON.parse(text);
}