/**
 * /api/v2/carbon-accounting — Issue #1432
 *
 * GHG Protocol corporate carbon-accounting API. Scope 1 (direct), Scope 2
 * (purchased energy, dual-reported) and Scope 3 (value chain) inventory
 * calculation with offsets reported separately, per the GHG Protocol
 * Corporate Standard.
 *
 * GET /api/v2/carbon-accounting
 *   Returns the emission-factor catalogue and scope definitions — every
 *   published factor used by the calculator, so results are reproducible.
 *
 *   Also accepts a simple scalar query subset to run an ad-hoc inventory
 *   without a body:
 *     ?scope1.naturalGasTherms=<n>
 *     &scope2.electricityKwh=<n>&scope2.gridRegion=<region>
 *     &scope2.renewablePercentage=<0..100>
 *     &offsets.creditsRetiredTonnes=<n>&offsets.treesPlanted=<n>
 *     &from=<ISO date>&to=<ISO date>
 *   When any of these are present, GET computes and returns an inventory
 *   report instead of the catalogue.
 *
 * POST /api/v2/carbon-accounting
 *   Content-Type: application/json — full request shape:
 *   {
 *     reportingPeriod?: { from, to }        // defaults to current calendar year
 *     organization?:  { name?, employees? } // employees enables intensity
 *     scope1?: { stationary?: [{ fuel, quantity }],
 *                mobile?:     [{ fuel, quantity }],
 *                fugitive?:   [{ refrigerant, quantityKg }] }
 *     scope2?: { electricityKwh?, heatKwh?, steamKwh?,
 *                gridRegion?, renewablePercentage? }
 *     scope3?: { freight?: [{ mode, tonnes, distanceKm }],
 *                waste?:   [{ method, tonnes }],
 *                businessTravel?: { shortHaulFlights?, longHaulFlights?,
 *                                   railKm?, carKm? },
 *                commuting?: [{ mode, distanceKm }] }
 *     offsets?: { creditsRetiredTonnes?, treesPlanted?, co2KgPerTree? }
 *   }
 *   At least one of scope1/scope2/scope3/offsets is required.
 *
 * Responses:
 *   200  GhgInventoryReport (POST/GET-with-params) | factor catalogue (GET)
 *   400  { error, details: string[] }  — validation failure
 *   500  { error }
 *
 * Closes #1432
 */

import { NextResponse } from 'next/server';
import {
  calculateGhgInventory,
  getGhgFactorCatalogue,
  parseGhgInventoryQuery,
  parseGhgInventoryRequest,
} from '@/lib/api/ghg-protocol';
import { apiVersionHeaders } from '@/lib/api/versioning';

export const runtime = 'nodejs';

// Inventories contain company operational data — never cache publicly.
const CACHE_HEADERS: Record<string, string> = {
  'Cache-Control': 'private, no-store, max-age=0',
};

function responseHeaders(): Record<string, string> {
  return {
    ...CACHE_HEADERS,
    ...(apiVersionHeaders('v2') as Record<string, string>),
  };
}

function validationResponse(details: string[]): NextResponse {
  return NextResponse.json(
    { error: 'Invalid carbon accounting request', details },
    { status: 400, headers: responseHeaders() }
  );
}

function errorResponse(error: unknown): NextResponse {
  console.error('[api/v2/carbon-accounting] error:', error);
  return NextResponse.json(
    { error: error instanceof Error ? error.message : 'Internal server error' },
    { status: 500, headers: responseHeaders() }
  );
}

function hasInventoryParams(searchParams: URLSearchParams): boolean {
  for (const key of searchParams.keys()) {
    if (key.startsWith('scope1.') || key.startsWith('scope2.') || key.startsWith('offsets.')) {
      return true;
    }
    if (key === 'from' || key === 'to') return true;
  }
  return false;
}

export function GET(request: Request): NextResponse {
  try {
    const url = new URL(request.url);

    if (hasInventoryParams(url.searchParams)) {
      const parsed = parseGhgInventoryQuery(url.searchParams);
      if (!parsed.ok) {
        return validationResponse(parsed.errors);
      }
      return NextResponse.json(calculateGhgInventory(parsed.data), {
        headers: responseHeaders(),
      });
    }

    return NextResponse.json(getGhgFactorCatalogue(), { headers: responseHeaders() });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function POST(request: Request): Promise<NextResponse> {
  try {
    let body: unknown;
    try {
      body = await request.json();
    } catch {
      return NextResponse.json(
        { error: 'Invalid JSON body' },
        { status: 400, headers: responseHeaders() }
      );
    }

    const parsed = parseGhgInventoryRequest(body);
    if (!parsed.ok) {
      return validationResponse(parsed.errors);
    }

    return NextResponse.json(calculateGhgInventory(parsed.data), {
      headers: responseHeaders(),
    });
  } catch (error) {
    return errorResponse(error);
  }
}
