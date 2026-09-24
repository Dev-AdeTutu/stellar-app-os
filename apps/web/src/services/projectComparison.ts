// Copyright 2024 Farm-credit Contributors
// Licensed under the Apache License, Version 2.0

/**
 * Project Comparison Service
 * 
 * Issue #1416: Compare multiple offset projects side-by-side:
 * - Price, co-benefits, methodology, verifier, risk rating, buyer reviews
 */

export interface Project {
  id: string;
  name: string;
  description: string;
  location: string;
  country: string;
  projectType: 'reforestation' | 'avoided_deforestation' | 'renewable_energy' | 'community' | 'blue_carbon';
  methodology: string;
  verifier: string;
  certification: string[];
  pricePerTon: number;
  currency: 'USD' | 'EUR' | 'GBP';
  vintage: string;
  totalCredits: number;
  availableCredits: number;
  riskRating: 'low' | 'medium' | 'high';
  coBenefits: CoBenefit[];
  images: string[];
  buyerReviews: BuyerReview[];
  createdAt: string;
  updatedAt: string;
}

export interface CoBenefit {
  category: 'biodiversity' | 'community' | 'water' | 'soil' | 'climate_resilience' | 'gender_equality';
  description: string;
  verified: boolean;
}

export interface BuyerReview {
  id: string;
  projectId: string;
  buyerName: string;
  buyerCompany?: string;
  rating: number; // 1-5
  title: string;
  content: string;
  verified: boolean;
  createdAt: string;
}

export interface ComparisonCriteria {
  id: string;
  label: string;
  key: keyof Project | 'coBenefits' | 'buyerReviews' | 'riskAssessment';
  type: 'text' | 'number' | 'rating' | 'tags' | 'boolean' | 'custom';
  sortable: boolean;
  defaultVisible: boolean;
}

export const COMPARISON_CRITERIA: ComparisonCriteria[] = [
  { id: 'name', label: 'Project Name', key: 'name', type: 'text', sortable: true, defaultVisible: true },
  { id: 'location', label: 'Location', key: 'location', type: 'text', sortable: true, defaultVisible: true },
  { id: 'projectType', label: 'Type', key: 'projectType', type: 'text', sortable: true, defaultVisible: true },
  { id: 'methodology', label: 'Methodology', key: 'methodology', type: 'text', sortable: false, defaultVisible: true },
  { id: 'verifier', label: 'Verifier', key: 'verifier', type: 'text', sortable: false, defaultVisible: true },
  { id: 'certification', label: 'Certifications', key: 'certification', type: 'tags', sortable: false, defaultVisible: true },
  { id: 'pricePerTon', label: 'Price/Ton', key: 'pricePerTon', type: 'number', sortable: true, defaultVisible: true },
  { id: 'vintage', label: 'Vintage', key: 'vintage', type: 'text', sortable: true, defaultVisible: false },
  { id: 'totalCredits', label: 'Total Credits', key: 'totalCredits', type: 'number', sortable: true, defaultVisible: false },
  { id: 'availableCredits', label: 'Available', key: 'availableCredits', type: 'number', sortable: true, defaultVisible: true },
  { id: 'riskRating', label: 'Risk Rating', key: 'riskRating', type: 'rating', sortable: true, defaultVisible: true },
  { id: 'coBenefits', label: 'Co-Benefits', key: 'coBenefits', type: 'custom', sortable: false, defaultVisible: true },
  { id: 'buyerReviews', label: 'Buyer Reviews', key: 'buyerReviews', type: 'custom', sortable: false, defaultVisible: true },
];

export interface ComparisonConfig {
  projectIds: string[];
  timeRange: '1y' | '3y' | '5y' | 'all';
  includeBaseline: boolean;
  includeSimilarCampaigns: boolean;
  maxSimilarCampaigns: number;
}

/**
 * Project Comparison Service
 */
export class ProjectComparisonService {
  private mockProjects: Map<string, Project> = new Map();

  constructor() {
    this.initializeMockData();
  }

  private initializeMockData(): void {
    const projects: Project[] = [
      {
        id: 'proj-001',
        name: 'Amazon Reforestation Project',
        description: 'Large-scale reforestation in the Amazon Basin restoring degraded lands.',
        location: 'Amazon Basin',
        country: 'Brazil',
        projectType: 'reforestation',
        methodology: 'VM0001',
        verifier: 'Verra',
        certification: ['VCS', 'CCB'],
        pricePerTon: 15.50,
        currency: 'USD',
        vintage: '2023',
        totalCredits: 100000,
        availableCredits: 50000,
        riskRating: 'low',
        coBenefits: [
          { category: 'biodiversity', description: 'Protects endangered species habitat', verified: true },
          { category: 'community', description: 'Employs 200 local workers', verified: true },
          { category: 'water', description: 'Protects watershed for downstream communities', verified: false },
        ],
        images: ['img1.jpg', 'img2.jpg'],
        buyerReviews: [
          { id: 'rev-1', projectId: 'proj-001', buyerName: 'Green Corp', rating: 5, title: 'Excellent', content: 'Great project with measurable impact', verified: true, createdAt: '2024-01-01' },
          { id: 'rev-2', projectId: 'proj-001', buyerName: 'Eco Inc', rating: 4, title: 'Good', content: 'Solid project with good reporting', verified: true, createdAt: '2024-02-01' },
        ],
        createdAt: '2024-01-01T00:00:00Z',
        updatedAt: '2024-01-01T00:00:00Z',
      },
      {
        id: 'proj-002',
        name: 'Kenya Mangrove Restoration',
        description: 'Mangrove restoration along the Kenyan coast for blue carbon.',
        location: 'Coastal Kenya',
        country: 'Kenya',
        projectType: 'blue_carbon',
        methodology: 'VM0033',
        verifier: 'Gold Standard',
        certification: ['VCS', 'CCB', 'SDVISTA'],
        pricePerTon: 22.00,
        currency: 'USD',
        vintage: '2023',
        totalCredits: 50000,
        availableCredits: 30000,
        riskRating: 'medium',
        coBenefits: [
          { category: 'biodiversity', description: 'Protects marine nursery habitats', verified: true },
          { category: 'community', description: 'Supports 150 local fisher families', verified: true },
          { category: 'climate_resilience', description: 'Protects coastline from storm surge', verified: true },
        ],
        images: ['img3.jpg'],
        buyerReviews: [
          { id: 'rev-3', projectId: 'proj-002', buyerName: 'Blue Carbon Fund', rating: 5, title: 'Outstanding', content: 'High quality blue carbon project', verified: true, createdAt: '2024-03-01' },
        ],
        createdAt: '2024-01-15T00:00:00Z',
        updatedAt: '2024-01-15T00:00:00Z',
      },
      {
        id: 'proj-003',
        name: 'Indonesia Peatland Restoration',
        description: 'Restoring degraded peatlands to prevent fires and emissions.',
        location: 'Sumatra',
        country: 'Indonesia',
        projectType: 'avoided_deforestation',
        methodology: 'VM0007',
        verifier: 'Verra',
        certification: ['VCS'],
        pricePerTon: 12.00,
        currency: 'USD',
        vintage: '2022',
        totalCredits: 75000,
        availableCredits: 40000,
        riskRating: 'medium',
        coBenefits: [
          { category: 'biodiversity', description: 'Protects orangutan habitat', verified: true },
          { category: 'climate_resilience', description: 'Prevents peat fires and haze', verified: true },
        ],
        images: ['img4.jpg'],
        buyerReviews: [
          { id: 'rev-4', projectId: 'proj-003', buyerName: 'Climate Fund X', rating: 4, title: 'Good', content: 'Important peatland protection', verified: true, createdAt: '2024-02-15' },
        ],
        createdAt: '2024-02-01T00:00:00Z',
        updatedAt: '2024-02-01T00:00:00Z',
      },
      {
        id: 'proj-004',
        name: 'Brazil Atlantic Forest Corridor',
        description: 'Connecting fragmented Atlantic Forest fragments.',
        location: 'Atlantic Forest',
        country: 'Brazil',
        projectType: 'reforestation',
        methodology: 'VM0016',
        verifier: 'Verra',
        certification: ['VCS', 'CCB'],
        pricePerTon: 18.00,
        currency: 'USD',
        vintage: '2023',
        totalCredits: 40000,
        availableCredits: 20000,
        riskRating: 'high',
        coBenefits: [
          { category: 'biodiversity', description: 'Connects jaguar corridors', verified: true },
        ],
        images: ['img5.jpg'],
        buyerReviews: [],
        createdAt: '2024-03-01T00:00:00Z',
        updatedAt: '2024-03-01T00:00:00Z',
      },
    ];

    for (const proj of projects) {
      this.mockProjects.set(proj.id, proj);
    }
  }

  async getProjects(filters?: {
    projectType?: string;
    country?: string;
    minPrice?: number;
    maxPrice?: number;
    riskRating?: string;
    verifier?: string;
  }): Promise<Project[]> {
    let projects = Array.from(this.mockProjects.values());

    if (filters?.projectType) {
      projects = projects.filter(p => p.projectType === filters.projectType);
    }
    if (filters?.country) {
      projects = projects.filter(p => p.country === filters.country);
    }
    if (filters?.minPrice) {
      projects = projects.filter(p => p.pricePerTon >= filters.minPrice!);
    }
    if (filters?.maxPrice) {
      projects = projects.filter(p => p.pricePerTon <= filters.maxPrice!);
    }
    if (filters?.riskRating) {
      projects = projects.filter(p => p.riskRating === filters.riskRating);
    }
    if (filters?.verifier) {
      projects = projects.filter(p => p.verifier === filters.verifier);
    }

    return projects;
  }

  async getProjectById(id: string): Promise<Project | null> {
    return this.mockProjects.get(id) || null;
  }

  async getComparisonProjects(ids: string[]): Promise<Project[]> {
    if (ids.length === 0) return [];
    const projects = await Promise.all(ids.map(id => this.getProjectById(id)));
    return projects.filter((p): p is Project => p !== null);
  }

  getComparisonCriteria(): ComparisonCriteria[] {
    return COMPARISON_CRITERIA;
  }

  getRiskColor(rating: Project['riskRating']): string {
    switch (rating) {
      case 'low': return 'text-green-600 bg-green-100';
      case 'medium': return 'text-yellow-600 bg-yellow-100';
      case 'high': return 'text-red-600 bg-red-100';
    }
  }

  getRiskIcon(rating: Project['riskRating']): string {
    switch (rating) {
      case 'low': return '✅';
      case 'medium': return '⚠️';
      case 'high': return '❌';
    }
  }

  formatPrice(price: number, currency: Project['currency'] = 'USD'): string {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency,
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(price);
  }

  getAverageRating(reviews: BuyerReview[]): number {
    if (reviews.length === 0) return 0;
    return reviews.reduce((sum, r) => sum + r.rating, 0) / reviews.length;
  }
}

export const projectComparisonService = new ProjectComparisonService();