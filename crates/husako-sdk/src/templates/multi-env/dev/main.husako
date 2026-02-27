import husako from "husako";
import { nginx } from "../base/nginx";
import { nginxService } from "../base/service";

husako.build([nginx("dev", 1, "nginx:latest"), nginxService("dev")]);
