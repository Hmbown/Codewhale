import { handleMerch } from "@/lib/merch/server";
export const dynamic = "force-dynamic";
export async function POST(request: Request) { return handleMerch(request, "quote"); }

