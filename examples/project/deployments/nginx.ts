import { labels } from "../lib";

export const nginx = {
  apiVersion: "apps/v1",
  kind: "Deployment",
  metadata: { name: "nginx", labels: labels("nginx") },
  spec: {
    replicas: 1,
    selector: { matchLabels: { app: "nginx" } },
    template: {
      metadata: { labels: { app: "nginx" } },
      spec: { containers: [{ name: "nginx", image: "nginx:1.25" }] },
    },
  },
};
