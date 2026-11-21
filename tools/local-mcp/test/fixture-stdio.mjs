import { serve } from "../server.mjs";

await serve(process.argv[2], { publicNotes: true });
