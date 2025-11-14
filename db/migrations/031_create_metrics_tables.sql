-- Migration 031: Create metrics tables for market value time series and pie chart data
-- This migration creates the caching tables needed for the metrics dashboard

-- Create metrics_market_value_daily table for time series data
CREATE TABLE IF NOT EXISTS metrics_market_value_daily (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    date_value DATE NOT NULL,
    growth_type VARCHAR(20) NOT NULL CHECK (growth_type IN ('market_growth', 'arda_growth')),
    filter_type VARCHAR(20) NOT NULL CHECK (filter_type IN ('all', 'originator', 'investor')),
    entity_id UUID NULL, -- company_id for originator/investor filters, null for 'all'
    cumulative_value DECIMAL(20,2) NOT NULL DEFAULT 0.00,
    deal_count INTEGER NOT NULL DEFAULT 0,
    calculated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    
    -- Unique constraint to prevent duplicates for the same date/type/filter combination
    UNIQUE(date_value, growth_type, filter_type, entity_id)
);

-- Create metrics_pie_chart_data table for distribution data
CREATE TABLE IF NOT EXISTS metrics_pie_chart_data (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chart_type VARCHAR(20) NOT NULL CHECK (chart_type IN ('industry', 'region', 'asset_class')),
    filter_type VARCHAR(20) NOT NULL CHECK (filter_type IN ('all', 'originator', 'investor')),
    entity_id UUID NULL, -- company_id for originator/investor filters, null for 'all'
    time_range VARCHAR(20) NOT NULL CHECK (time_range IN ('all_time', 'last_year')),
    category VARCHAR(255) NOT NULL, -- actual value like "Technology", "Real Estate", etc.
    value DECIMAL(20,2) NOT NULL DEFAULT 0.00, -- sum of principal amounts for this category
    percentage DECIMAL(5,2) NOT NULL DEFAULT 0.00, -- calculated percentage
    deal_count INTEGER NOT NULL DEFAULT 0,
    calculated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    
    -- Unique constraint to prevent duplicates for the same chart/filter/category combination
    UNIQUE(chart_type, filter_type, entity_id, time_range, category)
);

-- Add indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_metrics_market_value_daily_lookup 
    ON metrics_market_value_daily(growth_type, filter_type, entity_id, date_value);

CREATE INDEX IF NOT EXISTS idx_metrics_market_value_daily_date 
    ON metrics_market_value_daily(date_value DESC);

CREATE INDEX IF NOT EXISTS idx_metrics_pie_chart_data_lookup 
    ON metrics_pie_chart_data(chart_type, filter_type, entity_id, time_range);

CREATE INDEX IF NOT EXISTS idx_metrics_pie_chart_data_calculated_at 
    ON metrics_pie_chart_data(calculated_at DESC);

-- Add foreign key constraints for entity_id to companies table
-- Note: These are optional constraints - entity_id can be NULL for 'all' filter type
ALTER TABLE metrics_market_value_daily 
ADD CONSTRAINT fk_metrics_market_value_daily_entity_id 
FOREIGN KEY (entity_id) REFERENCES companies(id) ON DELETE CASCADE;

ALTER TABLE metrics_pie_chart_data 
ADD CONSTRAINT fk_metrics_pie_chart_data_entity_id 
FOREIGN KEY (entity_id) REFERENCES companies(id) ON DELETE CASCADE;

-- Add updated_at trigger function if it doesn't exist
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_proc WHERE proname = 'update_updated_at_column') THEN
        CREATE OR REPLACE FUNCTION update_updated_at_column()
        RETURNS TRIGGER AS '
        BEGIN
            NEW.updated_at = now();
            RETURN NEW;
        END;
        ' LANGUAGE plpgsql;
    END IF;
END $$;

-- Add update triggers
CREATE TRIGGER trigger_metrics_market_value_daily_updated_at
    BEFORE UPDATE ON metrics_market_value_daily
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trigger_metrics_pie_chart_data_updated_at
    BEFORE UPDATE ON metrics_pie_chart_data
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Add comments for documentation
COMMENT ON TABLE metrics_market_value_daily IS 'Cached daily cumulative market value data for time series charts';
COMMENT ON TABLE metrics_pie_chart_data IS 'Cached pie chart distribution data for industry, region, and asset class breakdowns';

COMMENT ON COLUMN metrics_market_value_daily.growth_type IS 'Type of growth calculation: market_growth (by originated_at) or arda_growth (by created_at)';
COMMENT ON COLUMN metrics_market_value_daily.filter_type IS 'Filter scope: all, originator, or investor';
COMMENT ON COLUMN metrics_market_value_daily.entity_id IS 'Company ID when filter_type is originator or investor, NULL for all';
COMMENT ON COLUMN metrics_market_value_daily.cumulative_value IS 'Cumulative sum of principal amounts up to this date';

COMMENT ON COLUMN metrics_pie_chart_data.chart_type IS 'Type of pie chart: industry, region, or asset_class';
COMMENT ON COLUMN metrics_pie_chart_data.time_range IS 'Time range filter: all_time or last_year';
COMMENT ON COLUMN metrics_pie_chart_data.category IS 'The category value (e.g., Technology, Real Estate, etc.)';
COMMENT ON COLUMN metrics_pie_chart_data.percentage IS 'Calculated percentage of total for this category';
