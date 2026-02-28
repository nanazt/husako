import { Deployment } from "k8s/apps/v1";
import { LabelSelector } from "k8s/_common";
import { name, namespace, label } from "k8s/meta/v1";
import { name as cname, cpu, memory, requests } from "k8s/core/v1";

export function nginx(ns: string, replicas: number, containerImage: string) {
  return Deployment()
    .metadata(
      name("nginx").namespace(ns).label("app", "nginx").label("env", ns),
    )
    .replicas(replicas)
    .selector(LabelSelector().matchLabels({ app: "nginx" }))
    .containers([
      cname("nginx")
        .image(containerImage)
        .resources(
          requests(cpu("250m").memory("128Mi")).limits(
            cpu("500m").memory("256Mi"),
          ),
        ),
    ]);
}
