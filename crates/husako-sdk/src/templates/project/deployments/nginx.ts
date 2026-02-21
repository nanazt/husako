import { Deployment } from "k8s/apps/v1";
import { name, label, cpu, memory, requests, limits } from "husako";

export const nginx = new Deployment()
  .metadata(name("nginx").label("app", "nginx"))
  .spec({
    replicas: 1,
    selector: { matchLabels: { app: "nginx" } },
    template: {
      metadata: { labels: { app: "nginx" } },
      spec: {
        containers: [{ name: "nginx", image: "nginx:1.25" }],
      },
    },
  })
  .resources(requests(cpu("250m").memory("128Mi")).limits(cpu("500m").memory("256Mi")));
