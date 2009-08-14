class CreateEvents < ActiveRecord::Migration
  def self.up
    create_table :events do |t|
      t.timestamp  "starts_at", :with_time_zone => true
      t.timestamp  "ends_at", :with_time_zone => true
      t.string   "timezone"
      t.integer  "utc_offset"
      t.string   "title"
      t.text     "description"
      t.string   "place"
      t.integer  "location_id"
      t.float    "latitude"
      t.float    "longitude"
      t.string   "url"
      t.string   "referring_link"
      t.integer  "recurrence_id"
      t.boolean  "is_all_day"
      t.boolean  "is_tenative"
      t.boolean  "is_cancelled"
      t.boolean  "is_accessible"
      t.integer  "parent_id"
      t.string   "privacy"
      t.integer  "calendar_id"
      t.timestamps
    end
  end

  def self.down
    drop_table :events
  end
end
