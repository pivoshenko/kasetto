import { serveInstallScript } from "@/lib/install-script";

export async function GET(request: Request) {
  return serveInstallScript(request, "install.ps1", "install-ps1");
}
