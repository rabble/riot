class CreateCalendars < ActiveRecord::Migration
  def self.up
    create_table :calendars do |t|
      t.string   "timezone"
      t.integer  "utc_offset"
      t.string   "title"
      t.text     "description"
      t.integer  "location_id"
      t.float    "latitude"
      t.float    "longitude"
      t.string   "url"
      t.timestamps
    end
  end

  def self.down
    drop_table :calendars
  end
end
