declare module 'envinfo' {
  const envinfo: { run(options: unknown, formatting?: { json?: boolean; showNotFound?: boolean }): Promise<string> };
  export default envinfo;
}
