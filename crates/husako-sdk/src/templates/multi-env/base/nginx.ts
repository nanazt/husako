import { Deployment } from "k8s/apps/v1";
import { name, namespace, label } from "husako";

export function nginx(ns: string, replicas: number, image: string) {
  return new Deployment()
    .metadata(
      name("nginx").namespace(ns).label("app", "nginx").label("env", ns)
    )
    .replicas(replicas)
    .selector({ matchLabels: { app: "nginx" } })
    .template({ metadata: { labels: { app: "nginx", env: ns } } })
    .containers([{
      name: "nginx",
      image,
      resources: {
        requests: { cpu: "250m", memory: "128Mi" },
        limits: { cpu: "500m", memory: "256Mi" },
      },
    }]);
}
