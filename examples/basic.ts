import { build } from "husako";

const deployment = {
  apiVersion: "apps/v1",
  kind: "Deployment",
  metadata: {
    name: "nginx",
    namespace: "default",
  },
  spec: {
    replicas: 3,
    selector: {
      matchLabels: { app: "nginx" },
    },
    template: {
      metadata: {
        labels: { app: "nginx" },
      },
      spec: {
        containers: [
          {
            name: "nginx",
            image: "nginx:1.25",
          },
        ],
      },
    },
  },
};

build([deployment]);
