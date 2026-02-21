// "k8s/apps/v1" module: Deployment

import { _ResourceBuilder } from "husako/_base";

export class Deployment extends _ResourceBuilder {
  constructor() {
    super("apps/v1", "Deployment");
  }
}
