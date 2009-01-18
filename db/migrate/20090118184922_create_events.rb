class CreateEvents < ActiveRecord::Migration
  def self.up
    create_table :events do |t|
      t.string   "start"
      t.datetime "start_date"
      t.string   "start_time"
      t.string   "end"
      t.datetime "end_date"
      t.string   "end_time"
      t.string   "title"
      t.text     "description"
      t.string   "location_name"
      t.string   "address"
      t.string   "latitude"
      t.string   "longitude"
      t.string   "url"
      t.string   "referring_link"
      t.timestamps
    end
  end

  def self.down
    drop_table :events
  end
end
