const http = require("http");

const PORT = process.env.PORT || 4000;

const server = http.createServer((req, res) => {
  if (req.url === "/health") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ status: "ok", service: "backend" }));
    return;
  }

  res.writeHead(200, { "Content-Type": "application/json" });
  res.end(JSON.stringify({ message: "API response", pid: process.pid }));
});

server.listen(PORT, () => console.log(`Backend API listening on port ${PORT}`));

process.on("SIGTERM", () => {
  console.log("Backend: SIGTERM received");
  server.close(() => process.exit(0));
});
