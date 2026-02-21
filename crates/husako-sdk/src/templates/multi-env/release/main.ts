import * as husako from "husako";
import { nginx } from "../base/nginx";
import { nginxService } from "../base/service";

husako.build([nginx("release", 3, "nginx:1.25"), nginxService("release")]);
