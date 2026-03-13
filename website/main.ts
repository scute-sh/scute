const GITHUB_REPO = "https://github.com/scute-sh/scute";
const INSTALLER_BASE = `${GITHUB_REPO}/releases/latest/download`;

const redirects: Record<string, string> = {
  "/": GITHUB_REPO,
  "/install": `${INSTALLER_BASE}/scute-installer.sh`,
  "/install.ps1": `${INSTALLER_BASE}/scute-installer.ps1`,
  "/docs": "https://docs.rs/scute",
};

Deno.serve((req) => {
  const { pathname } = new URL(req.url);
  const target = redirects[pathname];

  if (target) {
    return Response.redirect(target, 302);
  }

  return new Response("not found", { status: 404 });
});
