import { deployment } from "k8s/apps/v1";
import { container } from "k8s/core/v1";
import { selector } from "k8s/_common";
import { metadata, cpu, memory, requests, limits } from "husako";

export function nginx(ns: string, replicas: number, image: string) {
  return deployment()
    .metadata(
      metadata().name("nginx").namespace(ns).label("app", "nginx").label("env", ns)
    )
    .replicas(replicas)
    .selector(selector().matchLabels({ app: "nginx" }))
    .containers([
      container()
        .name("nginx")
        .image(image)
        .resources(
          requests(cpu("250m").memory("128Mi"))
            .limits(cpu("500m").memory("256Mi"))
        )
    ]);
}
