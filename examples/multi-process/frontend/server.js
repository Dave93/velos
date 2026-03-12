const http = require("http");

const PORT = process.env.PORT || 3000;

const server = http.createServer((req, res) => {
  if (req.url === "/health") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ status: "ok", service: "frontend" }));
    return;
  }

  res.writeHead(200, { "Content-Type": "text/html" });
  res.end("<h1>Velos Multi-Process Example</h1><p>Frontend running!</p>\n");
});

server.listen(PORT, () => console.log(`Frontend listening on port ${PORT}`));

process.on("SIGTERM", () => {
  console.log("Frontend: SIGTERM received");
  server.close(() => process.exit(0));
});
