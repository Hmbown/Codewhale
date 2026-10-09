import { handleMerch } from "@/lib/merch/server";
export const dynamic = "force-dynamic";
export async function GET(request: Request) { return handleMerch(request, "status"); }

