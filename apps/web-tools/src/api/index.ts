import { Hono } from "hono";
import { handleGithubAsset } from "./github-asset";

interface Env {
  ASSETS: Fetcher;
}

const app = new Hono<{ Bindings: Env }>();

app.get("/api/github-asset/:id{\\d+}", (c) => {
  return handleGithubAsset(c.req.param("id"));
});

app.all("*", (c) => {
  return c.env.ASSETS.fetch(c.req.raw);
});

export default app;
