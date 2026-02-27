import { Deployment } from "k8s/apps/v1";
import { LabelSelector } from "k8s/_common";
import { name, namespace, label } from "k8s/meta/v1";
import { name, image, cpu, memory, requests } from "k8s/core/v1";
import husako from "husako";

const nginx = Deployment()
  .metadata(name("nginx").namespace("default").label("app", "nginx"))
  .replicas(1)
  .selector(LabelSelector().matchLabels({ app: "nginx" }))
  .containers([
    name("nginx")
      .image("nginx:1.25")
      .resources(
        requests(cpu("250m").memory("128Mi")).limits(cpu("500m").memory("256Mi")),
      ),
  ]);

husako.build([nginx]);
