export async function GET() {
  const res = await fetch(
    "https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.sh"
  );
  return new Response(res.body, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}
